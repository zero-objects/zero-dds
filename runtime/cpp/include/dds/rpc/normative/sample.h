#ifndef OMG_DDS_RPC_INFRASTRUCTURE_H
#define OMG_DDS_RPC_INFRASTRUCTURE_H

#include <string>
#include <stdexcept>
#include <memory>
#include <iterator>

#ifdef OMG_DDS_RPC_REFERENCE_IMPLEMENTATION

#include "vendor_dependent.h"

#else 

namespace dds { 

  namespace details {

    template <bool, typename T, typename U>
    struct if_ {
      typedef T type;
    };

    template <typename T, typename U>
    struct if_ <false, T, U> {
      typedef U type;
    };

  } // namespace details

  template<typename T, bool> class SampleIterator;
  template<typename T>       class Sample;
  template<typename T>       class SampleRef;
  template<typename T>       class WriteSample;
  template<typename T>       class WriteSampleRef;
  template<class T>          class LoanedSamples;
  template<class T>          class SharedSamples;

  class SampleInfo;
  class SampleInfoSeq;
  class SampleIdentity;

  template <typename T> 
  struct dds_type_traits
  {
    typedef typename T::Seq               Seq;
    typedef const typename T::Seq         ConstSeq;
    typedef typename T::DataReader        DataReader;
    typedef typename T::DataWriter        DataWriter;

    typedef dds::SampleRef<T>             SampleRefType;
    typedef dds::SampleRef<T>             SampleIteratorValueType;
    typedef dds::SampleRef<const T>       ConstSampleIteratorValueType;
    typedef dds::LoanedSamples<T>         LoanedSamplesType;
    typedef dds::SharedSamples<T>         SharedSamplesType;
    typedef dds::SampleIterator<T, false> iterator;
    typedef dds::SampleIterator<T, true>  const_iterator;
  };

  template <> 
  struct dds_type_traits<dds::SampleInfo> 
  {
    typedef dds::SampleInfoSeq            Seq; 
    typedef const dds::SampleInfoSeq      ConstSeq;
  };

  struct dds_entity_traits; // defined in vendor_dependent.h

  template <typename T>
  class Sample 
  {
  public:
    typedef T                                             Data;
    typedef T&                                            DataReference;
    typedef const T&                                      ConstDataReference;
    typedef typename dds::dds_type_traits<T>::Seq         Seq;
    typedef typename dds::dds_type_traits<T>::DataReader  DataReader;
    typedef typename dds::dds_type_traits<T>::DataWriter  DataWriter;

    Sample();

    Sample(const Sample<T>& other);

    Sample(SampleRef<T> sample_ref);

    Sample(const T &data, const dds::SampleInfo& info);

    Sample & operator = (SampleRef<T> sample_ref);

    dds::SampleIdentity identity() const;

    dds::SampleIdentity related_identity() const;

    void swap(Sample & other);

    T& data();

    dds::SampleInfo& info();

    const T& data() const;

    const dds::SampleInfo& info() const;

    operator DataReference ();

    operator ConstDataReference () const;

    void set_data(const T &t);

    void set_data(const T *t);

    void set_info(const dds::SampleInfo &i);

    void set_info(const dds::SampleInfo *i);
  };

  template <typename T>
  class SampleRef
  {
  public:
    typedef T                                             Data;
    typedef T&                                            DataReference;
    typedef const T&                                      ConstDataReference;
    typedef typename dds::dds_type_traits<T>::Seq         Seq;
    typedef typename dds::dds_type_traits<T>::DataReader  DataReader;
    typedef typename dds::dds_type_traits<T>::DataWriter  DataWriter;

  public:
    SampleRef();

    SampleRef(T * data, dds::SampleInfo * info);

    SampleRef(T & data, dds::SampleInfo & info);

    SampleRef(Sample<T> & sample);

    SampleRef & operator = (Sample<T> & sample);

    T & data() const;

    dds::SampleInfo & info() const;

    operator T & () const;

    SampleRef<T> * operator ->();

    const SampleRef<T> * operator ->() const;

    void set_data(T * data);

    void set_data(T & data);

    void set_info(dds::SampleInfo * info);

    void set_info(dds::SampleInfo & info);

    bool is_nil_data() const;

    bool is_nil_info() const;

    void swap(SampleRef & other) throw();

    dds::SampleIdentity identity() const;

    dds::SampleIdentity related_identity() const;
  };

  template <typename T, bool IsConst>
  class SampleIterator 
  {
  public:
    typedef std::random_access_iterator_tag iterator_category;

    typedef typename details::if_<
      IsConst, 
      typename dds_type_traits<T>::ConstSampleIteratorValueType, 
      typename dds_type_traits<T>::SampleIteratorValueType>::type 
      value_type;

    typedef value_type reference;
    typedef value_type pointer;
    typedef std::ptrdiff_t difference_type;

    typedef typename details::if_<
      IsConst, 
      typename dds_type_traits<T>::ConstSeq, 
      typename dds_type_traits<T>::Seq>::type Seq;

    typedef typename details::if_<
      IsConst, 
      typename dds_type_traits<dds::SampleInfo>::ConstSeq, 
      typename dds_type_traits<dds::SampleInfo>::Seq>::type InfoSeq;

    SampleIterator();

    explicit SampleIterator(Seq & seq, 
      InfoSeq & info_seq, 
      int position = 0);

    SampleIterator (const SampleIterator<T, false> & si);

    SampleIterator & operator = (const SampleIterator<T, false> & si);

    value_type operator * () const;

    value_type operator ->() const;

    value_type operator [] (difference_type offset) const;

    SampleIterator & operator ++ ();

    SampleIterator operator ++ (int);

    SampleIterator & operator -- ();

    SampleIterator operator -- (int);

    SampleIterator & operator -= (difference_type i);

    SampleIterator & operator += (difference_type i);

    SampleIterator operator - (difference_type i) const;

    SampleIterator operator + (difference_type i) const;

    difference_type operator - (const SampleIterator & s2) const;

    bool operator < (const SampleIterator & s2) const;

    bool operator > (const SampleIterator & s2) const;

    bool operator <= (const SampleIterator & s2) const;

    bool operator >= (const SampleIterator & s2) const;

    bool operator == (const SampleIterator & s2) const;

    bool operator != (const SampleIterator & s2) const;

    bool is_end() const;
  };

  template <typename T>
  LoanedSamples<T> move(LoanedSamples<T> & ls) throw();

  // This class implements the move-constructor idiom as in DDS-PSM-Cxx
  template <typename T>
  class LoanedSamples
  {
  public:
    typedef typename dds_type_traits<T>::Seq                             Seq;
    typedef typename dds_type_traits<T>::DataReader                      DataReader;

    typedef SampleIterator<T, false>                                     iterator;
    typedef SampleIterator<T, true>                                      const_iterator;

    typedef typename dds_type_traits<T>::SampleIteratorValueType         value_type;
    typedef typename dds_type_traits<T>::ConstSampleIteratorValueType    const_value_type;
    typedef std::ptrdiff_t                                               difference_type;
  
  private:
    struct LoanMemento 
    {
      void * data;
    };

  public:

    LoanedSamples();

    static LoanedSamples<T> move_construct_from_loans(
      DataReader reader, 
      Seq & data_seq, 
      dds::SampleInfoSeq & info_seq);

    void release(
      DataReader & reader_ptr, 
      Seq & data_seq, 
      dds::SampleInfoSeq & info_seq); 

#ifdef OMG_DDS_RPC_HASCPP11    

    LoanedSamples(const LoanedSamples &) = delete;
    LoanedSamples & operator = (const LoanedSamples &) = delete;

    LoanedSamples(LoanedSamples &&) noexcept;
    LoanedSamples & operator = (LoanedSamples &&) noexcept;

#else
    LoanedSamples(LoanMemento loan_memento) throw();

    LoanedSamples & operator = (LoanMemento loan_memento) throw();

    operator LoanMemento () throw();

    friend LoanedSamples move<> (LoanedSamples &) throw();
#endif 

    ~LoanedSamples() throw();

    Seq & data_seq(); 

    dds::SampleInfoSeq & info_seq(); 

    const Seq & data_seq() const;

    const dds::SampleInfoSeq & info_seq() const;

    value_type operator [] (size_t index);

    const_value_type operator [] (size_t index) const;

    int length() const;

    void return_loan();

    void swap(LoanedSamples & that) throw();

    SharedSamples<T> to_shared() throw();

    iterator begin();

    iterator end();

    const_iterator begin() const;

    const_iterator end() const;
  };

  template <typename T>
  class SharedSamples
  {
  public:
    typedef typename dds_type_traits<T>::Seq                             Seq;
    typedef typename dds_type_traits<T>::DataReader                      DataReader;

    typedef SampleIterator<T, false>                                     iterator;
    typedef SampleIterator<T, true>                                      const_iterator;

    typedef typename dds_type_traits<T>::SampleIteratorValueType         value_type;
    typedef typename dds_type_traits<T>::ConstSampleIteratorValueType    const_value_type;
    typedef std::ptrdiff_t                                               difference_type;

  public:

    SharedSamples();

    SharedSamples(const SharedSamples &);

    SharedSamples & operator = (const SharedSamples &);

    ~SharedSamples() throw();

    Seq & data_seq(); 

    dds::SampleInfoSeq & info_seq(); 

    const Seq & data_seq() const;

    const dds::SampleInfoSeq & info_seq() const;

    value_type operator [] (size_t index);

    const_value_type operator [] (size_t index) const;

    int length() const;

    void swap(SharedSamples & that) throw();

    iterator begin();

    iterator end();

    const_iterator begin() const;

    const_iterator end() const;
  };

  template <typename T>
  class WriteSample  
  {
  public:

    WriteSample();

    WriteSample(const WriteSample<T>& other);

    WriteSample(const T & data);

    WriteSample(const WriteSampleRef<T> & wsref);

    WriteSample & operator = (const WriteSampleRef<T> & wsref);

    WriteSample & operator = (const WriteSample<T> & ws);

    const T & data() const;

    T & data();

    void set_data(const T & data);

    void set_identity(const dds::SampleIdentity & id);

    void swap(WriteSample & other) throw();

    dds::SampleIdentity identity() const;    
  };

  template <typename T>
  class WriteSampleRef 
  {
  public:

    WriteSampleRef();

    WriteSampleRef(const T & data);

    WriteSampleRef(WriteSample<T> & ws);

    WriteSampleRef(const WriteSampleRef<T> & wsref);

    WriteSampleRef & operator = (WriteSample<T> & ws);

    WriteSampleRef & operator = (const WriteSampleRef<T> & wsref);

    const T & data() const;

    T & data();

    void set_data(const T & data);

    void set_identity(const dds::SampleIdentity & id);

    void swap(WriteSampleRef & other) throw();

    dds::SampleIdentity identity() const;
  };


#ifdef OMG_DDS_RPC_HASCPP11    

  template <typename T>
  LoanedSamples<T> move(LoanedSamples<T> & ls) noexcept 
  {
    return std::move(ls);
  }

#else
  template <typename T>
  LoanedSamples<T> move(LoanedSamples<T> & ls) throw()
  {
    return LoanedSamples<T>(typename LoanedSamples<T>::LoanMemento(ls));
  }
#endif // OMG_DDS_RPC_HASCPP11

  template <typename T>
  typename LoanedSamples<T>::iterator begin(LoanedSamples<T> & ls)
  {
    return ls.begin();
  }

  template <typename T>
  typename LoanedSamples<T>::const_iterator begin(const LoanedSamples<T> & ls)
  {
    return ls.begin();
  }

  template <typename T>
  typename LoanedSamples<T>::iterator end(LoanedSamples<T> & ls)
  {
    return ls.end();
  }

  template <typename T>
  typename LoanedSamples<T>::const_iterator end(const LoanedSamples<T> & ls)
  {
    return ls.end();
  }

  template <typename T>
  void swap(LoanedSamples<T> &ls1, LoanedSamples<T> &ls2) throw()
  {
    ls1.swap(ls2);
  }

  template <typename T>
  typename SharedSamples<T>::iterator begin(SharedSamples<T> & ss)
  {
    return ss.begin();
  }

  template <typename T>
  typename SharedSamples<T>::const_iterator begin(const SharedSamples<T> & ss)
  {
    return ss.begin();
  }

  template <typename T>
  typename SharedSamples<T>::iterator end(SharedSamples<T> & ss)
  {
    return ss.end();
  }

  template <typename T>
  typename SharedSamples<T>::const_iterator end(const SharedSamples<T> & ss)
  {
    return ss.end();
  }

  template <typename T>
  void swap(SharedSamples<T> &ss1, SharedSamples<T> &ss2) throw()
  {
    ss1.swap(ss2);
  }

} // namespace dds 

#endif // OMG_DDS_RPC_REFERENCE_IMPLEMENTATION


#endif // OMG_DDS_RPC_INFRASTRUCTURE_H

